// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// `fixed<P,S>` as packed BCD, [CDR] §9.3.2.8 — the only wire definition
// an IDL fixed has. The value travels as decimal text, so the digit
// handling lives here and the reader's unpacking beside the writer's
// packing in `ZdReader.readFixed`.

/// The BCD nibbles of `text` under `fixed<digits, scale>`, sign nibble
/// last; `count` is "declared", "value-precision" or "value-string-digits"
/// — the integer digits of `text` as written plus S, made odd by one leading zero, which
/// ACE/TAO 2.5.24's `ACE_CDR::Fixed::from_string` writes, where OpenDDS writes the declared count
/// (`0.12345` under `fixed<5,5>` is `0012345c`; past 31 digits it is
/// refused). A decimal the declaration cannot hold is refused, which is
/// the sample's refusal.
func zdFixedNibbles(_ text: String, _ digits: Int, _ scale: Int, _ count: String, _ at: Int)
    throws -> [Int]
{
    if digits == 0 {
        try zdFailOutOfRange(at, "a fixed with at least one digit")
    }
    // OMG IDL 4.2 rule (42) caps the BCD rule at 31 digits, and
    // fixed<31, S> is the widest declaration IDL admits. A wider one has
    // no packing defined for it, so it is refused rather than packed into
    // octets no reader can be told the width of.
    if digits > 31 {
        try zdFailOutOfRange(at, "a fixed of at most 31 digits, not \(digits)")
    }
    if scale > digits {
        try zdFailOutOfRange(at, "a scale no larger than the digit count")
    }
    if count != "declared" && count != "value-precision" && count != "value-string-digits" {
        try zdFailOutOfRange(at, "a fixed digit count, not `\(count)`")
    }
    var negative = false
    var magnitude = Substring(text)
    if magnitude.hasPrefix("-") || magnitude.hasPrefix("+") {
        negative = magnitude.hasPrefix("-")
        magnitude = magnitude.dropFirst()
    }
    var whole = magnitude
    var fraction = Substring("")
    if let point = magnitude.firstIndex(of: ".") {
        whole = magnitude[magnitude.startIndex..<point]
        fraction = magnitude[magnitude.index(after: point)...]
    }
    func refuse() throws -> Never {
        try zdFailOutOfRange(at, "a decimal that fits fixed<\(digits), \(scale)>, not `\(text)`")
    }
    if whole.isEmpty && fraction.isEmpty {
        try refuse()
    }
    for c in whole where !c.isNumber || !c.isASCII {
        try refuse()
    }
    for c in fraction where !c.isNumber || !c.isASCII {
        try refuse()
    }
    let asWritten = whole.count + scale
    let integralWidth = digits - scale
    while whole.hasPrefix("0") {
        whole = whole.dropFirst()
    }
    if whole.count > integralWidth {
        try zdFailOutOfRange(
            at, "a decimal whose integer part fits \(integralWidth) digits, not `\(text)`")
    }
    if fraction.count > scale {
        try zdFailOutOfRange(at, "a decimal whose fraction fits \(scale) digits, not `\(text)`")
    }

    var every: [Int] = []
    for _ in 0..<(integralWidth - whole.count) {
        every.append(0)
    }
    for c in whole {
        every.append(c.wholeNumberValue ?? 0)
    }
    for c in fraction {
        every.append(c.wholeNumberValue ?? 0)
    }
    for _ in 0..<(scale - fraction.count) {
        every.append(0)
    }
    var significant = 0
    for (i, nibble) in every.enumerated() where nibble != 0 {
        significant = every.count - i
        break
    }
    var wire = every.count
    if count == "value-precision" {
        wire = max(1, significant)
    } else if count == "value-string-digits" {
        // ACE/TAO 2.5.24: the integer digits as written plus S, made odd by one leading
        // zero. Past 31 no reader can be told the field's width.
        wire = asWritten + (asWritten % 2 == 0 ? 1 : 0)
        if wire > 31 {
            try zdFailOutOfRange(at, "a fixed of at most 31 digits on the wire, not \(wire) for `\(text)`")
        }
    }
    var nibbles: [Int] = []
    if (wire + 1) % 2 == 1 {
        nibbles.append(0)
    }
    // A count wider than P is filled with leading zeros.
    let kept = min(wire, every.count)
    for _ in kept..<wire {
        nibbles.append(0)
    }
    nibbles.append(contentsOf: every.suffix(kept))
    var sign = 0x0C
    if negative && significant != 0 {
        sign = 0x0D
    }
    nibbles.append(sign)
    return nibbles
}
