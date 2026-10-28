// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// binary128 from and to decimal text, exactly.
//
// An IDL `long double` is held as decimal text in this column — Swift's
// extended type `Float80` exists only on x86 and `Double` has 53
// significand bits against binary128's 113 — so the two conversions
// live here: a correctly rounded parse (to nearest, ties to even, in
// one rounding), and the exact decimal expansion in the spelling the
// sibling columns' renderers produce.
//
// Swift's standard library has no arbitrary-precision integer, so the
// small one the two conversions need is below: unsigned, little-endian
// 32-bit limbs, and only the operations the conversions use.

// -- a minimal unsigned big integer ----------------------------------------

struct ZdBig {
    /// Little-endian 32-bit limbs, normalised: no trailing zero limb.
    var limbs: [UInt32]

    init(_ small: UInt64) {
        limbs = []
        if small != 0 {
            limbs.append(UInt32(truncatingIfNeeded: small))
            let high = UInt32(truncatingIfNeeded: small >> 32)
            if high != 0 {
                limbs.append(high)
            }
        }
    }

    /// From ASCII decimal digits; the caller has validated them.
    init(decimalDigits: String) {
        limbs = []
        for c in decimalDigits {
            multiplySmall(10)
            addSmall(UInt32(c.wholeNumberValue ?? 0))
        }
    }

    var isZero: Bool {
        return limbs.isEmpty
    }

    private mutating func normalise() {
        while let last = limbs.last, last == 0 {
            limbs.removeLast()
        }
    }

    var bitLength: Int {
        guard let top = limbs.last else {
            return 0
        }
        return (limbs.count - 1) * 32 + (32 - top.leadingZeroBitCount)
    }

    func bit(_ index: Int) -> Bool {
        let limb = index / 32
        if limb >= limbs.count {
            return false
        }
        return (limbs[limb] >> UInt32(index % 32)) & 1 == 1
    }

    mutating func setBit(_ index: Int) {
        let limb = index / 32
        while limbs.count <= limb {
            limbs.append(0)
        }
        limbs[limb] |= UInt32(1) << UInt32(index % 32)
    }

    mutating func addSmall(_ value: UInt32) {
        var carry = UInt64(value)
        var i = 0
        while carry != 0 {
            if i == limbs.count {
                limbs.append(0)
            }
            let sum = UInt64(limbs[i]) + carry
            limbs[i] = UInt32(truncatingIfNeeded: sum)
            carry = sum >> 32
            i += 1
        }
    }

    mutating func multiplySmall(_ value: UInt32) {
        var carry: UInt64 = 0
        for i in 0..<limbs.count {
            let product = UInt64(limbs[i]) * UInt64(value) + carry
            limbs[i] = UInt32(truncatingIfNeeded: product)
            carry = product >> 32
        }
        if carry != 0 {
            limbs.append(UInt32(truncatingIfNeeded: carry))
        }
        normalise()
    }

    /// This value times 10^power or 5^power.
    func scaled(by base: UInt32, power: Int) -> ZdBig {
        var out = self
        for _ in 0..<power {
            out.multiplySmall(base)
        }
        return out
    }

    func shiftedLeft(_ count: Int) -> ZdBig {
        if count <= 0 || isZero {
            return self
        }
        let limbShift = count / 32
        let bitShift = UInt32(count % 32)
        var out = ZdBig(0)
        out.limbs = [UInt32](repeating: 0, count: limbs.count + limbShift + 1)
        for i in 0..<limbs.count {
            let wide = UInt64(limbs[i]) << UInt64(bitShift)
            out.limbs[i + limbShift] |= UInt32(truncatingIfNeeded: wide)
            out.limbs[i + limbShift + 1] |= UInt32(truncatingIfNeeded: wide >> 32)
        }
        out.normalise()
        return out
    }

    func shiftedRight(_ count: Int) -> ZdBig {
        if count <= 0 || isZero {
            return self
        }
        let limbShift = count / 32
        if limbShift >= limbs.count {
            return ZdBig(0)
        }
        let bitShift = UInt32(count % 32)
        var out = ZdBig(0)
        out.limbs = [UInt32](repeating: 0, count: limbs.count - limbShift)
        for i in 0..<out.limbs.count {
            var value = limbs[i + limbShift] >> bitShift
            if bitShift != 0 && i + limbShift + 1 < limbs.count {
                value |= limbs[i + limbShift + 1] << (32 - bitShift)
            }
            out.limbs[i] = value
        }
        out.normalise()
        return out
    }

    func or(_ other: ZdBig) -> ZdBig {
        var out = self
        while out.limbs.count < other.limbs.count {
            out.limbs.append(0)
        }
        for i in 0..<other.limbs.count {
            out.limbs[i] |= other.limbs[i]
        }
        out.normalise()
        return out
    }

    func and(_ other: ZdBig) -> ZdBig {
        var out = ZdBig(0)
        let count = min(limbs.count, other.limbs.count)
        out.limbs = [UInt32](repeating: 0, count: count)
        for i in 0..<count {
            out.limbs[i] = limbs[i] & other.limbs[i]
        }
        out.normalise()
        return out
    }

    /// -1, 0 or 1 against `other`.
    func compare(_ other: ZdBig) -> Int {
        if limbs.count != other.limbs.count {
            return limbs.count < other.limbs.count ? -1 : 1
        }
        for i in stride(from: limbs.count - 1, through: 0, by: -1) {
            if limbs[i] != other.limbs[i] {
                return limbs[i] < other.limbs[i] ? -1 : 1
            }
        }
        return 0
    }

    /// Subtracts `other`, which the caller knows is no larger.
    mutating func subtract(_ other: ZdBig) {
        var borrow: Int64 = 0
        for i in 0..<limbs.count {
            let take = Int64(i < other.limbs.count ? other.limbs[i] : 0) + borrow
            var value = Int64(limbs[i]) - take
            if value < 0 {
                value += 0x1_0000_0000
                borrow = 1
            } else {
                borrow = 0
            }
            limbs[i] = UInt32(truncatingIfNeeded: value)
        }
        normalise()
    }

    /// Quotient and remainder against `divisor`, bit-serially: the two
    /// conversions run once per value, so clarity wins over speed.
    ///
    /// `divisor` is never zero in the one caller this has —
    /// `zdF128FromFraction`'s denominator is a power of ten — so this
    /// is a defensive check on the algorithm rather than a wire
    /// refusal; it still throws, and threads the write position it was
    /// given for, rather than trap, so a bug here reports the same way
    /// every other refusal in this file does instead of crashing the
    /// process outright.
    func divided(by divisor: ZdBig, _ at: Int) throws -> (quotient: ZdBig, remainder: ZdBig) {
        if divisor.isZero {
            try zdFailOutOfRange(at, "a nonzero divisor in the binary128 conversion")
        }
        var quotient = ZdBig(0)
        var remainder = ZdBig(0)
        for index in stride(from: bitLength - 1, through: 0, by: -1) {
            remainder = remainder.shiftedLeft(1)
            if bit(index) {
                remainder.addSmall(1)
            }
            if remainder.compare(divisor) >= 0 {
                remainder.subtract(divisor)
                quotient.setBit(index)
            }
        }
        return (quotient, remainder)
    }

    /// The decimal digits, most significant first; "0" for zero.
    func decimalString() -> String {
        if isZero {
            return "0"
        }
        var chunks: [UInt32] = []
        var current = self
        while !current.isZero {
            var remainder: UInt64 = 0
            var next = current
            for i in stride(from: next.limbs.count - 1, through: 0, by: -1) {
                let value = (remainder << 32) | UInt64(next.limbs[i])
                next.limbs[i] = UInt32(value / 1_000_000_000)
                remainder = value % 1_000_000_000
            }
            next.normalise()
            chunks.append(UInt32(remainder))
            current = next
        }
        var out = String(chunks.removeLast())
        for chunk in chunks.reversed() {
            let text = String(chunk)
            out += String(repeating: "0", count: 9 - text.count) + text
        }
        return out
    }

    /// The low sixteen bytes, least significant first.
    func lowSixteenBytes() -> [UInt8] {
        var out = [UInt8](repeating: 0, count: 16)
        for i in 0..<16 {
            let limb = i / 4
            if limb < limbs.count {
                out[i] = UInt8(truncatingIfNeeded: limbs[limb] >> UInt32(8 * (i % 4)))
            }
        }
        return out
    }
}

// -- binary128 -------------------------------------------------------------

private let zdF128FractionBits = 112
private let zdF128ExponentBias = 16383
private let zdF128ExponentMax = 0x7FFF

private func zdF128SignMask() -> ZdBig {
    var bit = ZdBig(0)
    bit.setBit(127)
    return bit
}

private func zdF128FractionMask() -> ZdBig {
    var mask = ZdBig(1).shiftedLeft(zdF128FractionBits)
    mask.subtract(ZdBig(1))
    return mask
}

private func zdF128ExponentBitsValue() -> ZdBig {
    return ZdBig(UInt64(zdF128ExponentMax)).shiftedLeft(zdF128FractionBits)
}

private func zdF128QuietBit() -> ZdBig {
    var bit = ZdBig(0)
    bit.setBit(zdF128FractionBits - 1)
    return bit
}

/// Parse `text` and answer the sixteen bytes, least-significant first.
/// A malformed decimal is refused at `at`.
func zdF128FromDecimal(_ text: String, _ at: Int) throws -> [UInt8] {
    func refuse() throws -> Never {
        try zdFailOutOfRange(at, "a decimal for a long double, not `\(text)`")
    }
    let stripped = text.trimmingWhitespace()
    switch stripped.lowercased() {
    case "inf", "+inf", "infinity", "+infinity":
        return zdF128ExponentBitsValue().lowSixteenBytes()
    case "-inf", "-infinity":
        return zdF128ExponentBitsValue().or(zdF128SignMask()).lowSixteenBytes()
    case "nan", "+nan":
        return zdF128ExponentBitsValue().or(zdF128QuietBit()).lowSixteenBytes()
    case "-nan":
        return zdF128ExponentBitsValue().or(zdF128QuietBit()).or(zdF128SignMask())
            .lowSixteenBytes()
    default:
        break
    }

    var negative = false
    var digitsPart = Substring(stripped)
    if digitsPart.hasPrefix("+") || digitsPart.hasPrefix("-") {
        negative = digitsPart.hasPrefix("-")
        digitsPart = digitsPart.dropFirst()
    }
    var mantissa = digitsPart
    var exponent10 = 0
    if let split = digitsPart.firstIndex(where: { $0 == "e" || $0 == "E" }) {
        mantissa = digitsPart[digitsPart.startIndex..<split]
        guard let parsed = Int(digitsPart[digitsPart.index(after: split)...]) else {
            try refuse()
        }
        exponent10 = parsed
    }
    var whole = mantissa
    var fraction = Substring("")
    if let point = mantissa.firstIndex(of: ".") {
        whole = mantissa[mantissa.startIndex..<point]
        fraction = mantissa[mantissa.index(after: point)...]
    }
    let digits = String(whole) + String(fraction)
    if digits.isEmpty {
        try refuse()
    }
    for c in digits where !(c.isASCII && c.isNumber) {
        // An underscore is refused rather than ignored: in a value file
        // it is a typo, not a digit separator.
        try refuse()
    }
    let significand = ZdBig(decimalDigits: digits)
    exponent10 -= fraction.count

    if significand.isZero {
        if negative {
            return zdF128SignMask().lowSixteenBytes()
        }
        return ZdBig(0).lowSixteenBytes()
    }
    // The exact rational, then one correct rounding on it: a two-step
    // parse through a double rounds twice.
    var numerator = significand
    var denominator = ZdBig(1)
    if exponent10 >= 0 {
        numerator = numerator.scaled(by: 10, power: exponent10)
    } else {
        denominator = denominator.scaled(by: 10, power: -exponent10)
    }
    return try zdF128FromFraction(negative, numerator, denominator, at).lowSixteenBytes()
}

/// Round numerator/denominator to binary128, to nearest with ties to
/// even: scale so the quotient carries more bits than binary128 needs,
/// fold everything discarded into one sticky bit, round once.
private func zdF128FromFraction(
    _ negative: Bool, _ numerator: ZdBig, _ denominator: ZdBig, _ at: Int
) throws -> ZdBig {
    let wanted = 130
    let shift = max(0, wanted - (numerator.bitLength - denominator.bitLength))
    let scaled = numerator.shiftedLeft(shift)
    var (quotient, remainder) = try scaled.divided(by: denominator, at)
    if !remainder.isZero {
        quotient.setBit(0)
    }
    return zdF128Compose(negative, quotient, -shift)
}

/// Build a pattern from a sign, an integral significand and a binary
/// exponent, rounding to nearest with ties to even.
private func zdF128Compose(_ negative: Bool, _ significand: ZdBig, _ exponent: Int) -> ZdBig {
    let sign = negative ? zdF128SignMask() : ZdBig(0)
    if significand.isZero {
        return sign
    }
    var exponent = exponent
    if exponent > zdF128ExponentMax {
        return sign.or(zdF128ExponentBitsValue())
    }
    if exponent < -(zdF128FractionBits + 1 + zdF128ExponentBias + 200) {
        return sign
    }
    var value = significand
    let leading = value.bitLength - 1
    if leading > zdF128FractionBits {
        let drop = leading - zdF128FractionBits
        value = zdF128ShiftRound(value, drop)
        exponent += drop
        if value.bitLength - 1 > zdF128FractionBits {
            value = value.shiftedRight(1)
            exponent += 1
        }
    } else {
        let shift = zdF128FractionBits - leading
        value = value.shiftedLeft(shift)
        exponent -= shift
    }
    let biased = exponent + zdF128ExponentBias + zdF128FractionBits
    if biased >= zdF128ExponentMax {
        return sign.or(zdF128ExponentBitsValue())
    }
    if biased <= 0 {
        // Subnormal; a value that rounds up to the smallest normal
        // carries into the exponent field on its own.
        return sign.or(zdF128ShiftRound(value, 1 - biased))
    }
    return sign.or(ZdBig(UInt64(biased)).shiftedLeft(zdF128FractionBits))
        .or(value.and(zdF128FractionMask()))
}

/// Shift right, rounding to nearest with ties to even.
private func zdF128ShiftRound(_ value: ZdBig, _ shift: Int) -> ZdBig {
    if shift <= 0 {
        return value
    }
    var kept = value.shiftedRight(shift)
    var mask = ZdBig(1).shiftedLeft(shift)
    mask.subtract(ZdBig(1))
    let remainder = value.and(mask)
    if remainder.isZero {
        return kept
    }
    var half = ZdBig(0)
    half.setBit(shift - 1)
    let against = remainder.compare(half)
    if against > 0 || (against == 0 && kept.bit(0)) {
        kept.addSmall(1)
    }
    return kept
}

/// Every digit the value has, in the corpus's canonical form — the
/// exact expansion, in exactly the spelling the sibling renderers
/// produce, because the columns are compared as text.
func zdF128ToExactDecimal(_ bytes: [UInt8]) -> String {
    var bits = ZdBig(0)
    bits.limbs = [UInt32](repeating: 0, count: 4)
    for i in 0..<16 {
        bits.limbs[i / 4] |= UInt32(bytes[i]) << UInt32(8 * (i % 4))
    }
    bits.limbs.reverseNormalise()

    let negative = !bits.and(zdF128SignMask()).isZero
    let biasedBig = bits.shiftedRight(zdF128FractionBits).and(ZdBig(UInt64(zdF128ExponentMax)))
    let biased = biasedBig.limbs.first.map(Int.init) ?? 0
    let fraction = bits.and(zdF128FractionMask())

    if biased == zdF128ExponentMax {
        if !fraction.isZero {
            return "nan"
        }
        return negative ? "-inf" : "inf"
    }
    var significand: ZdBig
    var exponent: Int
    if biased == 0 {
        significand = fraction
        exponent = 1 - zdF128ExponentBias - zdF128FractionBits
    } else {
        var implicit = ZdBig(0)
        implicit.setBit(zdF128FractionBits)
        significand = fraction.or(implicit)
        exponent = biased - zdF128ExponentBias - zdF128FractionBits
    }
    if significand.isZero {
        return negative ? "-0.0" : "0.0"
    }
    var digits: String
    var point: Int
    if exponent >= 0 {
        digits = significand.shiftedLeft(exponent).decimalString()
        point = digits.count
    } else {
        // m * 2^-k is exactly m * 5^k * 10^-k.
        let scale = -exponent
        digits = significand.scaled(by: 5, power: scale).decimalString()
        point = digits.count - scale
    }
    while digits.hasSuffix("0") {
        digits.removeLast()
    }
    if digits.isEmpty {
        digits = "0"
    }
    return zdF128Render(negative, digits, point)
}

/// 0.<digits> * 10^point, in the three forms all the columns use.
private func zdF128Render(_ negative: Bool, _ digits: String, _ point: Int) -> String {
    let sign = negative ? "-" : ""
    if 0 < point && point <= 34 {
        if digits.count <= point {
            return sign + digits + String(repeating: "0", count: point - digits.count) + ".0"
        }
        return sign + digits.prefix(point) + "." + digits.dropFirst(point)
    }
    if -6 < point && point <= 0 {
        return sign + "0." + String(repeating: "0", count: -point) + digits
    }
    var mantissa = String(digits.prefix(1)) + "."
    if digits.count > 1 {
        mantissa += digits.dropFirst()
    } else {
        mantissa += "0"
    }
    let exponent = point - 1
    if exponent >= 0 {
        return sign + mantissa + "e+" + String(exponent)
    }
    return sign + mantissa + "e-" + String(-exponent)
}

extension Array where Element == UInt32 {
    /// Drops trailing zero limbs — a bare-array sibling of
    /// `ZdBig.normalise`, for the one site that fills limbs directly.
    fileprivate mutating func reverseNormalise() {
        while let last = self.last, last == 0 {
            self.removeLast()
        }
    }
}

extension String {
    /// The value with ASCII whitespace stripped from both ends, without
    /// reaching for Foundation.
    fileprivate func trimmingWhitespace() -> String {
        var view = Substring(self)
        while let first = view.first, first == " " || first == "\t" || first == "\n"
            || first == "\r"
        {
            view = view.dropFirst()
        }
        while let last = view.last, last == " " || last == "\t" || last == "\n" || last == "\r" {
            view = view.dropLast()
        }
        return String(view)
    }
}
