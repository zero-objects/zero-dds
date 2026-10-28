// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// Taking bytes off the wire: the mirror of the writer clause for
// clause. A malformed stream throws a `ZdWireError`; a refusal about
// the type sets the status and the generated decoder walks on.

/// Reads one sample's bytes.
public final class ZdReader {
    private let data: [UInt8]
    private var at = 0
    private let endianness: ZdEndianness
    private let maxAlign: Int
    private var origin = 0
    private let framing: ZdPlCdr1Framing
    private var status = ZdStatus.ok

    /// A reader over `data`, expecting one implementation's PL_CDR1
    /// framing whole. The reader takes the whole framing rather than
    /// its parts, so a caller cannot set one and forget the other; what
    /// it reads off it is the flag policy, the short-id ceiling and the
    /// declared-length convention.
    public init(
        _ data: [UInt8],
        _ endianness: ZdEndianness,
        _ maxAlign: Int,
        _ framing: ZdPlCdr1Framing
    ) {
        self.data = data
        self.endianness = endianness
        self.maxAlign = max(1, maxAlign)
        self.framing = framing
    }

    // -- the status a generated decoder reads ------------------------------

    /// Whether a refusal has been made, so a later framing read does
    /// nothing.
    private var stopped: Bool {
        return status != .ok
    }

    /// How the read is going.
    public func readerStatus() -> ZdStatus {
        return status
    }

    /// Refuse the stream from outside the runtime; the first refusal
    /// sticks, and `.ok` refuses nothing.
    public func readerRefuse(_ refusal: ZdStatus) {
        if refusal != .ok && status == .ok {
            status = refusal
        }
    }

    /// Take back a `.discarded`, and only that one: a truncated stream
    /// is not a narrowing.
    public func readerClearDiscard() -> Bool {
        if status == .discarded {
            status = .ok
            return true
        }
        return false
    }

    /// Where the reader is.
    public func readerPosition() -> Int {
        return at
    }

    /// How many bytes are left.
    public func readerRemaining() -> Int {
        if at > data.count {
            return 0
        }
        return data.count - at
    }

    /// Move the cursor forward to `target`, never backwards: a framing
    /// word pointing behind the cursor is a refusal about the stream's
    /// shape, and past the end fails.
    public func readerSkipTo(_ target: Int) throws {
        if stopped {
            return
        }
        if target < at {
            readerRefuse(.outOfRange)
            return
        }
        if target > data.count {
            try zdFailTruncated(at, target - at, readerRemaining())
        }
        at = target
    }

    /// The cursor, the origin and the status, to be put back later.
    public func readerSave() -> ZdReaderMark {
        return ZdReaderMark(at: at, origin: origin, status: status)
    }

    /// Put back what `readerSave` took.
    public func readerRestore(_ mark: ZdReaderMark) {
        at = mark.at
        origin = mark.origin
        status = mark.status
    }

    /// Move the alignment origin to the current position and answer the
    /// origin displaced, for `readerFinishPackedElement`. The read-side
    /// half of `beginPackedElement`; see it for the axis and the
    /// recorded rows.
    public func readerBeginPackedElement() -> Int {
        let saved = origin
        origin = at
        return saved
    }

    /// Restore the origin `readerBeginPackedElement` displaced.
    public func readerFinishPackedElement(_ saved: Int) {
        origin = saved
    }

    // -- the cursor --------------------------------------------------------

    private func alignTo(_ alignment: Int) {
        let capped = min(alignment, maxAlign)
        if capped <= 1 {
            return
        }
        let offset = at - origin
        at += (capped - offset % capped) % capped
    }

    private func take(_ count: Int) throws -> ArraySlice<UInt8> {
        let end = at + count
        if end > data.count {
            try zdFailTruncated(at, count, readerRemaining())
        }
        let chunk = data[at..<end]
        at = end
        return chunk
    }

    private func readUnsigned(_ width: Int) throws -> UInt64 {
        alignTo(width)
        let raw = Array(try take(width))
        var value: UInt64 = 0
        if endianness == .little {
            for i in stride(from: width - 1, through: 0, by: -1) {
                value = (value << 8) | UInt64(raw[i])
            }
        } else {
            for i in 0..<width {
                value = (value << 8) | UInt64(raw[i])
            }
        }
        return value
    }

    // -- primitives --------------------------------------------------------

    /// `boolean`; a byte that is neither 0 nor 1 is refused rather than
    /// coerced.
    public func readBool() throws -> Bool {
        let position = at
        let value = try take(1).first ?? 0
        if value == 0 {
            return false
        }
        if value == 1 {
            return true
        }
        try zdFailOutOfRange(position, "a boolean of 0 or 1")
    }

    /// `octet` and `uint8`.
    public func readU8() throws -> UInt8 {
        return try take(1).first ?? 0
    }

    /// `int8`.
    public func readI8() throws -> Int8 {
        return Int8(bitPattern: try take(1).first ?? 0)
    }

    /// `char`: one octet.
    public func readChar() throws -> UInt8 {
        return try take(1).first ?? 0
    }

    /// `short`.
    public func readI16() throws -> Int16 {
        return Int16(bitPattern: UInt16(truncatingIfNeeded: try readUnsigned(2)))
    }

    /// `unsigned short`.
    public func readU16() throws -> UInt16 {
        return UInt16(truncatingIfNeeded: try readUnsigned(2))
    }

    /// `long`.
    public func readI32() throws -> Int32 {
        return Int32(bitPattern: UInt32(truncatingIfNeeded: try readUnsigned(4)))
    }

    /// `unsigned long`.
    public func readU32() throws -> UInt32 {
        return UInt32(truncatingIfNeeded: try readUnsigned(4))
    }

    /// `long long`.
    public func readI64() throws -> Int64 {
        return Int64(bitPattern: try readUnsigned(8))
    }

    /// `unsigned long long`.
    public func readU64() throws -> UInt64 {
        return try readUnsigned(8)
    }

    /// `float`.
    public func readF32() throws -> Float {
        return Float(bitPattern: UInt32(truncatingIfNeeded: try readUnsigned(4)))
    }

    /// `double`.
    public func readF64() throws -> Double {
        return Double(bitPattern: try readUnsigned(8))
    }

    /// `long double`: all sixteen bytes, as the exact decimal the
    /// generated member holds.
    public func readF128() throws -> String {
        alignTo(8)
        let raw = Array(try take(16))
        var bytes = [UInt8](repeating: 0, count: 16)
        if endianness == .little {
            for i in 0..<16 {
                bytes[i] = raw[i]
            }
        } else {
            for i in 0..<16 {
                bytes[i] = raw[15 - i]
            }
        }
        return zdF128ToExactDecimal(bytes)
    }

    /// `wchar`, at `width` octets and in `framing`'s shape. The
    /// four-byte form zero-extends one UTF-16 code unit; a value that
    /// uses the high bits, and a lone surrogate, are refused.
    public func readWchar(_ width: Int, _ framing: String, _ order: String) throws -> UInt16 {
        let position = at
        if width != 2 && width != 4 {
            try zdFailOutOfRange(position, "a wchar width of 2 or 4")
        }
        var value: UInt32
        if framing == "giop-length-prefixed" {
            let declared = try readU8()
            if Int(declared) != width {
                try zdFailOutOfRange(
                    position,
                    "a GIOP 1.2 wchar of \(width) octets, the configured transmission "
                        + "code set, and not \(declared)")
            }
            let big = order != "stream" || endianness != .little
            let raw = Array(try take(width))
            value = 0
            if big {
                for i in 0..<width {
                    value = (value << 8) | UInt32(raw[i])
                }
            } else {
                for i in stride(from: width - 1, through: 0, by: -1) {
                    value = (value << 8) | UInt32(raw[i])
                }
            }
        } else if framing != "fixed" {
            try zdFailOutOfRange(position, "a wchar framing, not `\(framing)`")
        } else if width == 4 {
            value = try readU32()
            if value > 0xFFFF {
                try zdFailOutOfRange(position, "a four-byte wchar holding one UTF-16 code unit")
            }
        } else {
            value = UInt32(try readU16())
        }
        if value >= 0xD800 && value <= 0xDFFF {
            try zdFailOutOfRange(position, "a Unicode code point")
        }
        return UInt16(truncatingIfNeeded: value)
    }

    // -- strings -----------------------------------------------------------

    /// A string: a length including the terminator, the bytes, the NUL.
    public func readString() throws -> String {
        let position = at
        let length = Int(try readU32())
        if length == 0 || length > readerRemaining() + 1 {
            try zdFailImpossibleLength(position, length, readerRemaining())
        }
        let text = Array(try take(length - 1))
        guard let candidate = String(bytes: text, encoding: .utf8) else {
            try zdFailNotUtf8(position)
        }
        _ = try readU8()
        return candidate
    }

    /// `readString` under the name the shared walk calls it by: a Swift
    /// string owns its storage, so the two names are one method.
    public func readStringAlloc() throws -> String {
        return try readString()
    }

    /// A 16-bit value little-endian whatever the stream's order is.
    public func readU16Little() throws -> UInt16 {
        alignTo(2)
        let raw = Array(try take(2))
        return UInt16(raw[0]) | (UInt16(raw[1]) << 8)
    }

    /// A 16-bit value big-endian whatever the stream's order is.
    public func readU16Big() throws -> UInt16 {
        alignTo(2)
        let raw = Array(try take(2))
        return (UInt16(raw[0]) << 8) | UInt16(raw[1])
    }

    /// A 32-bit value little-endian whatever the stream's order is.
    public func readU32Little() throws -> UInt32 {
        alignTo(4)
        let raw = Array(try take(4))
        return UInt32(raw[0]) | (UInt32(raw[1]) << 8) | (UInt32(raw[2]) << 16)
            | (UInt32(raw[3]) << 24)
    }

    /// A 32-bit value big-endian whatever the stream's order is.
    public func readU32Big() throws -> UInt32 {
        alignTo(4)
        let raw = Array(try take(4))
        return (UInt32(raw[0]) << 24) | (UInt32(raw[1]) << 16) | (UInt32(raw[2]) << 8)
            | UInt32(raw[3])
    }

    /// A wstring, in the form `policy` and `order` name, answered as a
    /// Swift `String` — which is what the generated member holds. The
    /// form is never inferred from the stream.
    public func readWstringUtf8(_ policy: String, _ order: String) throws -> String {
        let position = at
        let declared = Int(try readU32())
        if !zdWstringOrderKnown(order) {
            try zdFailOutOfRange(position, "a wstring unit order, not `\(order)`")
        }
        guard let cell = zdWstringCellOf(policy) else {
            try zdFailOutOfRange(position, "a wstring length policy, not `\(policy)`")
        }
        var count = declared
        if !cell.countsCharacters {
            if declared % cell.width != 0 {
                try zdFailImpossibleLength(position, declared, readerRemaining())
            }
            count = declared / cell.width
        }
        var units = try wstringUnits(position, count, cell.width, order)
        if cell.terminated {
            if declared < 1 {
                try zdFailImpossibleLength(position, declared, readerRemaining())
            }
            let last = units.removeLast()
            if last != 0 {
                try zdFailOutOfRange(
                    position, "a NUL character ending the wstring, as its count includes one")
            }
            for unit in units where unit == 0 {
                try zdFailOutOfRange(position, "no NUL inside a wstring a C peer would stop at")
            }
        }
        if order == "byte-order-mark" {
            units = zdStripByteOrderMark(units)
        }
        var scalars = String.UnicodeScalarView()
        var index = 0
        while index < units.count {
            let unit = units[index]
            if unit >= 0xD800 && unit <= 0xDBFF && index + 1 < units.count {
                let low = units[index + 1]
                if low >= 0xDC00 && low <= 0xDFFF {
                    let point =
                        0x10000 + ((UInt32(unit) - 0xD800) << 10) + (UInt32(low) - 0xDC00)
                    scalars.append(Unicode.Scalar(point) ?? Unicode.Scalar(0))
                    index += 2
                    continue
                }
            }
            if unit >= 0xD800 && unit <= 0xDFFF {
                try zdFailOutOfRange(position, "a UTF-16 sequence with no lone surrogate")
            }
            scalars.append(Unicode.Scalar(UInt32(unit)) ?? Unicode.Scalar(0))
            index += 1
        }
        return String(scalars)
    }

    /// The inverse of `writeWstringUtf8Octets`: the declared length is
    /// the UTF-8 octet count, the terminator `policy` asks for is the
    /// last of those octets, and a count the stream cannot hold, a
    /// missing or an interior NUL, or an ill-formed UTF-8 sequence is
    /// refused. See `WstringCodeUnits::Utf8`.
    public func readWstringUtf8Octets(_ policy: String) throws -> String {
        let position = at
        let declared = Int(try readU32())
        guard let cell = zdWstringCellOf(policy) else {
            try zdFailOutOfRange(position, "a wstring length policy, not `\(policy)`")
        }
        if declared > readerRemaining() {
            try zdFailImpossibleLength(position, declared, readerRemaining())
        }
        var data = Array(try take(declared))
        if cell.terminated {
            if declared < 1 || data[declared - 1] != 0 {
                try zdFailOutOfRange(
                    position, "a NUL octet ending the wstring, as its count includes one")
            }
            data.removeLast()
            for octet in data where octet == 0 {
                try zdFailOutOfRange(
                    position, "no NUL inside a wstring a C peer would stop at")
            }
        }
        guard let text = String(bytes: data, encoding: .utf8) else {
            try zdFailNotUtf8(position)
        }
        return text
    }

    private func wstringUnits(_ position: Int, _ count: Int, _ width: Int, _ order: String)
        throws -> [UInt16]
    {
        if count * width > readerRemaining() {
            try zdFailImpossibleLength(position, count * width, readerRemaining())
        }
        let little = order == "byte-order-mark" || order == "host-unswapped"
        var units: [UInt16] = []
        units.reserveCapacity(count)
        for _ in 0..<count {
            if width == 4 {
                let wide: UInt32
                if order == "big-endian-always" {
                    wide = try readU32Big()
                } else if little {
                    wide = try readU32Little()
                } else {
                    wide = try readU32()
                }
                if wide > 0xFFFF {
                    try zdFailOutOfRange(
                        position, "a four-byte wstring character holding one UTF-16 unit")
                }
                units.append(UInt16(truncatingIfNeeded: wide))
                continue
            }
            if order == "big-endian-always" {
                units.append(try readU16Big())
            } else if little {
                units.append(try readU16Little())
            } else {
                units.append(try readU16())
            }
        }
        return units
    }

    /// `fixed<digits, scale>`, as the decimal string the writer took:
    /// the leading zeros go, and a negative zero loses its sign. Under
    /// "value-precision" and "value-string-digits" the field ends at the first octet whose low
    /// nibble is a sign, and one that never presents a sign within the
    /// sixteen octets of the widest fixed IDL admits is refused.
    public func readFixed(_ digits: Int, _ scale: Int, _ count: String) throws -> String {
        if count != "declared" && count != "value-precision" && count != "value-string-digits" {
            try zdFailOutOfRange(at, "a fixed digit count, not `\(count)`")
        }
        // The same bound as the writer's, on this side too: OMG IDL 4.2
        // rule (42) caps the BCD rule at 31 digits, and a reader asked for
        // a wider field would take octets no declaration can name.
        if digits <= 0 || digits > 31 || scale > digits {
            try zdFailOutOfRange(
                at,
                "a fixed<P, S> with S <= P and 1 <= P <= 31, "
                    + "not fixed<\(digits), \(scale)>")
        }
        var nibbles: [Int] = []
        let sign: Int
        if count == "value-precision" || count == "value-string-digits" {
            while true {
                if nibbles.count >= 32 {
                    try zdFailOutOfRange(at, "a sign nibble within sixteen octets")
                }
                let b = try take(1).first ?? 0
                nibbles.append(Int(b >> 4))
                nibbles.append(Int(b & 0x0F))
                if (b & 0x0F) >= 0x0A {
                    break
                }
            }
            sign = nibbles.removeLast()
        } else {
            let raw = Array(try take((digits + 2) / 2))
            for b in raw {
                nibbles.append(Int(b >> 4))
                nibbles.append(Int(b & 0x0F))
            }
            sign = nibbles.removeLast()
            if (digits + 1) % 2 == 1 {
                nibbles.removeFirst()
            }
        }
        for nibble in nibbles where nibble > 9 {
            try zdFailOutOfRange(at, "BCD digits")
        }
        var text = nibbles.map { String($0) }.joined()
        while text.count < scale + 1 {
            text = "0" + text
        }
        let split = text.count - scale
        var whole = String(text.prefix(split))
        while whole.count > 1 && whole.hasPrefix("0") {
            whole.removeFirst()
        }
        let fraction = String(text.suffix(scale))
        var result = whole
        if scale > 0 {
            result += "." + fraction
        }
        var zero = whole == "0"
        for digit in fraction where digit != "0" {
            zero = false
        }
        if sign == 0x0D && !zero {
            return "-" + result
        }
        return result
    }

    // -- the trims a bound reaches for -------------------------------------

    /// The trim, reached as a method because the shared walk emits it
    /// as a call on the reader.
    public func trimUtf8(_ text: String, _ octets: Int) -> String {
        return zdTrimUtf8(text, octets)
    }

    /// The wide half.
    public func trimUtf16(_ text: String, _ units: Int) -> String {
        return zdTrimUtf16(text, units)
    }

    // -- the bit-bounded kinds ---------------------------------------------

    /// An enumerator's holder, as the signed integer §7.4.3.5.3 makes
    /// it — the holder's width and not the bound's.
    public func readEnum(_ bitBound: Int) throws -> Int64 {
        if bitBound == 0 || bitBound > 32 {
            try zdFailOutOfRange(at, "an @bit_bound between 1 and 32, not \(bitBound)")
        }
        if bitBound <= 8 {
            return Int64(Int8(bitPattern: try readU8()))
        }
        if bitBound <= 16 {
            return Int64(Int16(bitPattern: try readU16()))
        }
        return Int64(Int32(bitPattern: try readU32()))
    }

    /// A bitmask's holder. Table 33 of §7.4.1.1.3.2.
    public func readBitmask(_ bitBound: Int) throws -> UInt64 {
        if bitBound == 0 || bitBound > 64 {
            try zdFailOutOfRange(at, "an @bit_bound between 1 and 64, not \(bitBound)")
        }
        if bitBound <= 8 {
            return UInt64(try readU8())
        }
        if bitBound <= 16 {
            return UInt64(try readU16())
        }
        if bitBound <= 32 {
            return UInt64(try readU32())
        }
        return try readU64()
    }

    /// A bitset's anchor integer, of the width the planner sized.
    public func readBitset(_ anchor: Int) throws -> UInt64 {
        switch anchor {
        case 1:
            return UInt64(try readU8())
        case 2:
            return UInt64(try readU16())
        case 4:
            return UInt64(try readU32())
        case 8:
            return try readU64()
        default:
            try zdFailOutOfRange(at, "a bitset anchor of 1, 2, 4 or 8 bytes, not \(anchor)")
        }
    }

    // -- framing -----------------------------------------------------------

    /// A DHEADER: the length the members that follow occupy, refusing
    /// one the stream cannot hold.
    public func readDheader() throws -> UInt32 {
        let position = at
        let length = try readU32()
        if Int(length) > readerRemaining() {
            try zdFailImpossibleLength(position, Int(length), readerRemaining())
        }
        return length
    }

    /// An EMHEADER, unpacked. Codes 5 to 7 report the NEXTINT they
    /// read, but the reader is put back before it: that word is the
    /// member's own leading length. §7.4.3.4.2.
    public func readEmheader() throws -> ZdEmHeaderRead {
        let word = try readU32()
        var header = ZdEmHeaderRead()
        header.memberId = Int(word & zdEMemberIdMask)
        header.lengthCode = Int((word >> 28) & 0x7)
        header.mustUnderstand = (word >> 31) != 0
        header.nextInt = -1
        if header.lengthCode == 4 {
            header.nextInt = Int(try readU32())
        } else if header.lengthCode >= 5 {
            let position = at
            header.nextInt = Int(try readU32())
            at = position
        }
        header.extent = ZdReader.emExtent(header)
        return header
    }

    private static func emExtent(_ header: ZdEmHeaderRead) -> Int {
        if header.lengthCode <= 3 {
            return 1 << header.lengthCode
        }
        if header.nextInt < 0 {
            return 0
        }
        if header.lengthCode == 4 {
            return header.nextInt
        }
        var scale = 1
        if header.lengthCode == 6 {
            scale = 4
        } else if header.lengthCode == 7 {
            scale = 8
        }
        return 4 + header.nextInt * scale
    }

    private func readPidHeader(_ role: ZdParameterRole) throws -> ZdPidRead {
        alignTo(4)
        let pid = Int(try readU16())
        let length = Int(try readU16())
        if (pid & zdPidIdMask) == (zdPidExtended & zdPidIdMask) {
            // The four flag bits are not part of the id: §7.4.1.2.1
            // splits the eMemberHeader into FLAG_1 to FLAG_4 and a
            // 28-bit member id, and FLAG_2 is the member's
            // must-understand.
            let memberHeader = try readU32()
            let wideLength = Int(try readU32())
            var header = ZdPidRead()
            header.pid = Int(memberHeader & zdEMemberIdMask)
            header.length = wideLength
            header.end = false
            header.mustUnderstand = (memberHeader & zdEFlagMustUnderstand) != 0
            header.implExtension = (memberHeader & zdEFlagImplExtension) != 0
            header.role = role
            return header
        }
        let identifier = pid & zdPidIdMask
        let end = identifier == (zdPidListEnd & zdPidIdMask)
        var header = ZdPidRead()
        header.pid = identifier
        header.length = length
        header.end = end
        header.mustUnderstand = (pid & zdPidMustUnderstand) != 0
        // Under a framing that puts FLAG_IMPL_EXTENSION on every member
        // id the bit carries no information and is reported absent.
        header.implExtension =
            try (pid & zdPidImplExtension) != 0
            && (zdPidFlagsAt(framing.pidFlags, "member") & zdPidImplExtension) == 0
        header.reserved = !end && identifier > framing.highestShortId
        header.ignore = identifier == zdPidIgnore
        header.role = role
        return header
    }

    /// A PL_CDR1 parameter header in the member role.
    public func readPid() throws -> ZdPidRead {
        return try readPidAs(.member)
    }

    /// A parameter header, and the value block it opens: §7.4.1.2 makes
    /// a parameter's value its own stream block, so a member's internal
    /// alignment is counted from the first octet after the header.
    public func readPidAs(_ role: ZdParameterRole) throws -> ZdPidRead {
        if stopped {
            var header = ZdPidRead()
            header.end = true
            header.role = role
            header.savedOrigin = origin
            header.valueStart = at
            return header
        }
        var header = try readPidHeader(role)
        header.savedOrigin = origin
        header.valueStart = at
        origin = at
        return header
    }

    /// End a parameter's value and step to the next header: the origin
    /// goes back, and the padding to four is stepped against the
    /// enclosing stream. Nothing follows PID_LIST_END, and a reader
    /// that has already refused does not move.
    ///
    /// The declared length is checked against the configured framing
    /// rather than believed: a length belonging to the other convention
    /// is a stream this reader was pointed at with the wrong framing,
    /// and reading on would put every later member at an offset no
    /// writer produced. Both readings are accepted where they coincide,
    /// which is every value already a multiple of four.
    public func finishPidRead(_ header: ZdPidRead) {
        origin = header.savedOrigin
        if stopped || header.end {
            return
        }
        let used = at - header.valueStart
        let declared = header.length
        let padded = (used + 3) & ~3
        let pads = framing.padsDeclaredLength(header.role)
        if declared != (pads ? padded : used) && declared != used && declared != padded {
            readerRefuse(.outOfRange)
        }
        alignTo(4)
    }

    /// Step over a member this reader does not know.
    ///
    /// XTypes 1.3 §7.4.3.4.2: codes 0 to 3 state a size of one, two,
    /// four or eight bytes outright; code 4 is followed by a NEXTINT
    /// holding the member's length; and codes 5, 6 and 7 have a NEXTINT
    /// that is *also* the member's own leading word, so the member
    /// spans that word plus the length it states, times one, four or
    /// eight.
    public func skipMember(_ header: ZdEmHeaderRead) throws {
        if stopped {
            return
        }
        try readerSkipTo(at + header.extent)
    }
}

func zdStripByteOrderMark(_ units: [UInt16]) -> [UInt16] {
    if units.isEmpty {
        return units
    }
    if units[0] == zdByteOrderMark {
        return Array(units.dropFirst())
    }
    var rest = units
    if units[0] == zdSwappedByteOrderMark {
        rest = Array(units.dropFirst())
    }
    return rest.map { ($0 << 8) | ($0 >> 8) }
}
