// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The Swift wire runtime: DDS-XTypes 1.3 §7.4 from the clause.
//
// The counterpart of crates/wire, runtime/c, runtime/d and the other
// columns — a further implementation of the same clauses, not a binding
// to any of them. Where two columns disagree on an octet, one of them
// has misread the specification, and that is the point of having both.
// Nothing here decides a type question: which members are written, in
// what order, under which header and with which length code arrives as
// arguments from the generated codec.
//
// A malformed stream or a value outside its type throws a `ZdWireError`
// — the refusal is the sample's, the same one the sibling runtimes
// raise as an exception — while a refusal about the *type* sets the
// reader's status and the generated decoder walks on to the end of the
// sample.

import Foundation

/// What can go wrong on the wire.
///
/// The variants of `crates/wire/src/error.rs`, `runtime/typescript`'s
/// `WireError`, `runtime/java`'s `WireException` and
/// `runtime/python`'s `WireError`, as a Swift `Error`. Every one names
/// what was expected and where: a reader that returned a zero for a
/// truncated stream would hand the caller a value that was never sent,
/// and a writer that clamped an over-wide integer would put a
/// different number on the wire than the caller asked for.
public enum ZdWireError: Error {
    /// The stream ended before the value did.
    case truncated(at: Int, needed: Int, available: Int)
    /// A length field claims more than the stream holds.
    ///
    /// Separate from `truncated`: a length that cannot be true is a
    /// malformed stream, not a short one, and allocating for it is how
    /// a reader turns a bad message into an out-of-memory.
    case impossibleLength(at: Int, claimed: Int, available: Int)
    /// A string held bytes that are not UTF-8.
    case notUtf8(at: Int)
    /// A value outside what its type admits.
    ///
    /// Thrown by the writer as well as the reader: an out-of-range
    /// sample the application handed the writer is refused here, the
    /// same as a value the wire cannot hold on the way in.
    case outOfRange(at: Int, expected: String)
}

extension ZdWireError: CustomStringConvertible {
    /// The exact sentence the sibling runtimes' exception messages
    /// carry, so a caller that prints `"\(error)"` reads the same
    /// refusal in every column.
    public var description: String {
        switch self {
        case let .truncated(at, needed, available):
            return "at \(at): \(needed) bytes needed, \(available) available"
        case let .impossibleLength(at, claimed, available):
            return "at \(at): a length of \(claimed) with \(available) bytes left"
        case let .notUtf8(at):
            return "at \(at): the string is not UTF-8"
        case let .outOfRange(at, expected):
            return "at \(at): expected \(expected)"
        }
    }
}

/// The stream ended before the value did.
public func zdFailTruncated(_ at: Int, _ needed: Int, _ available: Int) throws -> Never {
    throw ZdWireError.truncated(at: at, needed: needed, available: available)
}

/// A length field claims more than the stream holds. Separate from the
/// truncated refusal: a length that cannot be true is a malformed
/// stream, not a short one.
public func zdFailImpossibleLength(_ at: Int, _ claimed: Int, _ available: Int) throws -> Never {
    throw ZdWireError.impossibleLength(at: at, claimed: claimed, available: available)
}

/// A string held bytes that are not UTF-8.
public func zdFailNotUtf8(_ at: Int) throws -> Never {
    throw ZdWireError.notUtf8(at: at)
}

/// A value outside what its type admits.
public func zdFailOutOfRange(_ at: Int, _ expected: String) throws -> Never {
    throw ZdWireError.outOfRange(at: at, expected: expected)
}

// -- policies --------------------------------------------------------------

/// The byte order a stream is written or read in.
public enum ZdEndianness {
    case little
    case big
}

/// What a receiving position does with a value it cannot hold.
/// DDS-XTypes 1.3 §7.2.2.7, Table 11.
public enum ZdTryConstruct {
    case discard
    case useDefault
    case trim
}

/// What a reader answers about the type, without unwinding.
public enum ZdStatus {
    case ok
    case truncated
    case impossibleLength
    case outOfRange
    case discarded
}

/// A status, spelled the way the sibling runtimes log it.
public func zdStatusText(_ status: ZdStatus) -> String {
    switch status {
    case .ok: return "ok"
    case .truncated: return "truncated"
    case .impossibleLength: return "impossible-length"
    case .outOfRange: return "out-of-range"
    case .discarded: return "discarded"
    }
}

/// Which of §7.4.1.2's two roles a parameter header stands in.
public enum ZdParameterRole {
    case member
    case optional
}

/// The parameter ids and flags of XTypes 1.3 §7.4.1.2.2 and
/// DDSI-RTPS 2.5 §9.4.2.11.2.
public let zdPidExtended = 0x3F01
/// Ends a parameter list.
public let zdPidListEnd = 0x3F02
/// The highest member id a short header carries, inclusive.
public let zdPidShortMaxId = 0x3F00
/// FLAG_IMPL_EXTENSION, bit 15 of a short id.
public let zdPidImplExtension = 0x8000
/// The id every consumer shall ignore.
public let zdPidIgnore = 0x3F03
/// The longest value a short header can state.
public let zdPidShortMaxLen = 0xFFFF
/// The must-understand bit of a short id.
public let zdPidMustUnderstand = 0x4000

let zdEFlagMustUnderstand: UInt32 = 0x4000_0000
let zdEFlagImplExtension: UInt32 = 0x8000_0000
let zdEMemberIdMask: UInt32 = 0x0FFF_FFFF
let zdPidIdMask = 0x3FFF

/// What flags a PL_CDR1 parameter id carries at `position` — "member",
/// "sentinel" or "extended-marker". An unknown policy fails loudly: a
/// typo in a wire-affecting setting must be loud.
func zdPidFlagsAt(_ policy: String, _ position: String) throws -> Int {
    if policy == "sentinel-must-understand" {
        if position == "sentinel" || position == "extended-marker" {
            return zdPidMustUnderstand
        }
        return 0
    }
    if policy == "bare" {
        return 0
    }
    if policy == "impl-extension-members" {
        if position == "member" || position == "extended-marker" {
            return zdPidImplExtension | zdPidMustUnderstand
        }
        return 0
    }
    try zdFailOutOfRange(0, "a PID-flag policy, not `\(policy)`")
}

/// How a PL_CDR1 parameter list is framed — the ways the measured
/// implementations differ inside an XCDR1 sample.
public struct ZdPlCdr1Framing {
    /// What flags a parameter id carries, asked per position.
    public let pidFlags: String
    /// The highest member id a short parameter header carries. Both
    /// sides: `writePid` refuses an id above it rather than putting a
    /// reserved parameter on the wire.
    public let highestShortId: Int
    /// Whether a @mutable member's declared length covers its padding.
    public let padDeclaredLength: Bool
    /// The same question for an @optional member outside @mutable.
    public let padDeclaredOptionalLength: Bool
    /// Whether a member's own must-understand bit goes out at all.
    public let memberMustUnderstand: Bool
    /// Whether the parameter writes its own padding to four after the
    /// value — the clause's framing — or leaves the next header to
    /// align itself, which is Fast DDS's.
    public let padAfterValue: Bool

    public init(
        _ pidFlags: String,
        _ highestShortId: Int,
        _ padDeclaredLength: Bool,
        _ padDeclaredOptionalLength: Bool,
        _ memberMustUnderstand: Bool,
        _ padAfterValue: Bool
    ) {
        self.pidFlags = pidFlags
        self.highestShortId = highestShortId
        self.padDeclaredLength = padDeclaredLength
        self.padDeclaredOptionalLength = padDeclaredOptionalLength
        self.memberMustUnderstand = memberMustUnderstand
        self.padAfterValue = padAfterValue
    }

    /// The clause's framing, which is also what a reader with no
    /// stated framing expects.
    public static func defaults() -> ZdPlCdr1Framing {
        return ZdPlCdr1Framing("sentinel-must-understand", zdPidShortMaxId, false, false, true, true)
    }

    func padsDeclaredLength(_ role: ZdParameterRole) -> Bool {
        if role == .optional {
            return padDeclaredOptionalLength
        }
        return padDeclaredLength
    }
}

// -- the wstring product ---------------------------------------------------

/// One cell of the wstring C1 x C2 x C3 product.
struct ZdWstringCell {
    let countsCharacters: Bool
    let terminated: Bool
    let width: Int
}

/// The five named forms and the three coordinate spellings; the
/// spellings are `WstringLengthPolicy::as_str`'s. `nil` where the name
/// is not a policy at all.
func zdWstringCellOf(_ policy: String) -> ZdWstringCell? {
    switch policy {
    case "specification", "octets/unterminated/2":
        return ZdWstringCell(countsCharacters: false, terminated: false, width: 2)
    case "character-count", "characters/unterminated/2":
        return ZdWstringCell(countsCharacters: true, terminated: false, width: 2)
    case "character-count-terminated", "characters/terminated/4":
        return ZdWstringCell(countsCharacters: true, terminated: true, width: 4)
    case "specification-wide-units", "octets/unterminated/4":
        return ZdWstringCell(countsCharacters: false, terminated: false, width: 4)
    case "ros-type-support", "characters/unterminated/4":
        return ZdWstringCell(countsCharacters: true, terminated: false, width: 4)
    case "octets/terminated/2":
        return ZdWstringCell(countsCharacters: false, terminated: true, width: 2)
    case "octets/terminated/4":
        return ZdWstringCell(countsCharacters: false, terminated: true, width: 4)
    case "characters/terminated/2":
        return ZdWstringCell(countsCharacters: true, terminated: true, width: 2)
    default:
        return nil
    }
}

func zdWstringOrderKnown(_ order: String) -> Bool {
    return order == "stream" || order == "big-endian-always"
        || order == "byte-order-mark" || order == "host-unswapped"
}

let zdByteOrderMark: UInt16 = 0xFEFF
let zdSwappedByteOrderMark: UInt16 = 0xFFFE
