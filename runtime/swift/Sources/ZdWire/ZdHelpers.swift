// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The helpers the generated code names beside the writer and the
// reader: conversions, bounds measures, code-point checks, the union
// overlay's bit patterns, and the JSON spellings the dump side shares.

// -- conversions -----------------------------------------------------------

/// The walk's cast, spelled once: every conversion the shared codec
/// walk emits goes through here, because Swift has no implicit numeric
/// conversion and no conversion at all between an integer and a raw
/// enumeration.
///
/// `truncatingIfNeeded`, not the trapping initialiser: the walk casts
/// where it knows the value fits — an enum holder it has ranged, a
/// masked bitset field — and where it deliberately keeps storage bits,
/// as the union overlay does with a negative arm. A trap there would
/// turn a defined narrowing into a crash.
public func zdTo<T: FixedWidthInteger, V: BinaryInteger>(_ type: T.Type, _ value: V) -> T {
    return T(truncatingIfNeeded: value)
}

/// The same cast from a raw-representable holder — an enumeration or an
/// `OptionSet` bitmask — whose integer is its `rawValue`.
public func zdTo<T: FixedWidthInteger, V: RawRepresentable>(_ type: T.Type, _ value: V) -> T
where V.RawValue: BinaryInteger {
    return T(truncatingIfNeeded: value.rawValue)
}

/// The cast the other way: an integer into a raw-representable type.
/// Only ever reached for a value the walk has already matched against
/// the type's declared constants — a union label, a ranged enumerator —
/// so the failable requirement cannot fail here. That is a
/// code-generation invariant and not a wire refusal — the value is a
/// literal the generator wrote, never a byte off the network — so a
/// violation traps rather than throws: throwing here would put `throws`
/// on every conversion this seam handles, for a branch nothing on the
/// wire can reach.
public func zdTo<T: RawRepresentable, V: BinaryInteger>(_ type: T.Type, _ value: V) -> T
where T.RawValue: FixedWidthInteger {
    guard let made = T(rawValue: T.RawValue(truncatingIfNeeded: value)) else {
        preconditionFailure("zdTo: no declared value of \(T.self) is \(value)")
    }
    return made
}

// -- measures and trims ----------------------------------------------------

/// C11 §7.24.4's unsigned-octet order, as the sign the walk compares.
/// Swift's UTF-8 views compare octet by octet, which is exactly that
/// order.
public func zdStrcmp(_ a: String, _ b: String) -> Int {
    var left = a.utf8.makeIterator()
    var right = b.utf8.makeIterator()
    while true {
        switch (left.next(), right.next()) {
        case (nil, nil):
            return 0
        case (nil, _):
            return -1
        case (_, nil):
            return 1
        case (let x?, let y?):
            if x < y { return -1 }
            if x > y { return 1 }
        }
    }
}

/// How many octets `text` is, which is what a string<N> bound counts.
public func zdUtf8Octets(_ text: String) -> Int {
    return text.utf8.count
}

/// How many UTF-16 code units `text` is, which is what a wstring<N>
/// bound counts. A scalar above the basic plane is two.
public func zdUtf16Units(_ text: String) -> Int {
    return text.utf16.count
}

/// `text` cut to at most `octets` bytes, never inside a character.
/// Table 11: the order is preserved, so this is a prefix.
public func zdTrimUtf8(_ text: String, _ octets: Int) -> String {
    var used = 0
    var out = String()
    for character in text {
        let size = String(character).utf8.count
        if used + size > octets {
            break
        }
        used += size
        out.append(character)
    }
    return out
}

/// `text` cut to at most `units` UTF-16 code units.
public func zdTrimUtf16(_ text: String, _ units: Int) -> String {
    var used = 0
    var out = String()
    for character in text {
        let cost = String(character).utf16.count
        if used + cost > units {
            break
        }
        used += cost
        out.append(character)
    }
    return out
}

// -- code points -----------------------------------------------------------

/// `point` as the one octet an IDL char is, refused above U+00FF rather
/// than truncated: the refusal is the sample's — the same one the
/// sibling runtimes raise.
public func zdCheckedChar(_ point: UInt32) throws -> UInt8 {
    if point > 0xFF {
        try zdFailOutOfRange(0, "a character below U+0100")
    }
    return UInt8(point)
}

/// `point` as the one UTF-16 code unit an IDL wchar is, refused outside
/// the BMP. §7.2.2.2.1.2.5.
public func zdCheckedWchar(_ point: UInt32) throws -> UInt16 {
    if point > 0xFFFF {
        try zdFailOutOfRange(0, "a BMP character")
    }
    return UInt16(point)
}

/// The UTF-16 code units of `v`, without a BOM. A Swift `String` is
/// always well-formed Unicode, so unlike the byte-holding columns there
/// is no invalid-UTF-8 refusal to make here.
func zdUtf16UnitsOf(_ v: String) -> [UInt16] {
    return Array(v.utf16)
}

// -- the union overlay's bit patterns --------------------------------------

/// The low `octets` of `value`'s two's-complement bits, zero-extended —
/// `writeUnionOverlay`'s pattern for an integer, character, enumeration
/// or bitmask arm. The mask is the function: a plain widening of a
/// negative value sign-extends, and `-2` in four octets has to be
/// 0x00000000FFFFFFFE.
public func zdBitPatternInt(_ value: Int64, _ octets: Int) -> UInt64 {
    let pattern = UInt64(bitPattern: value)
    if octets >= 8 {
        return pattern
    }
    return pattern & ((1 << (UInt64(octets) * 8)) - 1)
}

/// One octet, 0 or 1 — the boolean overlay pattern.
public func zdBitPatternBool(_ value: Bool) -> UInt64 {
    return value ? 1 : 0
}

/// The IEEE 754 binary32 bits, zero-extended.
public func zdBitPatternF32(_ value: Float) -> UInt64 {
    return UInt64(value.bitPattern)
}

/// The IEEE 754 binary64 bits.
public func zdBitPatternF64(_ value: Double) -> UInt64 {
    return UInt64(value.bitPattern)
}

// -- the JSON spellings the dump side shares -------------------------------

/// Whether a float has a JSON number form at all: NaN and the
/// infinities are not JSON.
public func zdJsonNumberOK(_ v: Double) -> Bool {
    return !v.isNaN && !v.isInfinite
}

/// `text` as a JSON string, quoted and escaped.
public func zdJsonString(_ text: String) -> String {
    var out = "\""
    for scalar in text.unicodeScalars {
        switch scalar {
        case "\"":
            out += "\\\""
        case "\\":
            out += "\\\\"
        case "\n":
            out += "\\n"
        case "\r":
            out += "\\r"
        case "\t":
            out += "\\t"
        default:
            if scalar.value < 0x20 {
                let hex = String(scalar.value, radix: 16)
                out += "\\u" + String(repeating: "0", count: 4 - hex.count) + hex
            } else {
                out.unicodeScalars.append(scalar)
            }
        }
    }
    out += "\""
    return out
}

/// The padding §7.4.1.1 puts after serialized data of `length` octets,
/// whose count §7.6.3.1.2 records in the encapsulation options.
public func zdEncapsulationPadding(_ length: Int) -> Int {
    return (4 - length % 4) % 4
}
