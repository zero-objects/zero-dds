// SPDX-License-Identifier: Apache-2.0
// Copyright 2026 ZeroDDS Contributors
//
// The small values the runtime hands a generated codec: a header
// waiting for its length on the way out, a header as read on the way
// back, and the mark a two-pass read puts back.

/// A DHEADER waiting for its length; `at` below zero is the delimiter a
/// @final aggregate does not write.
public struct ZdDHeader {
    public var at: Int = -1

    public init(_ at: Int = -1) {
        self.at = at
    }
}

/// The DHEADER a value that takes no delimiter does not write; the
/// writer's `finishDheader` ignores it.
public func zdNoDheader() -> ZdDHeader {
    return ZdDHeader(-1)
}

/// An EMHEADER waiting for its NEXTINT; `nextInt` is -1 where the
/// length code carries the length itself.
public struct ZdEmHeader {
    public var nextInt: Int = -1
    public var word: Int = 0
    /// The code the header was written with, so that `finishEmheader`
    /// can hold the member's value to the length the code declares.
    /// Codes 0 to 3 are lengths; anything else declares nothing of its
    /// own.
    public var lengthCode: Int = 8

    public init(_ nextInt: Int = -1, _ word: Int = 0, _ lengthCode: Int = 8) {
        self.nextInt = nextInt
        self.word = word
        self.lengthCode = lengthCode
    }
}

/// A parameter header waiting for its length.
public struct ZdPid {
    public var at: Int = 0
    public var extended: Bool = false
    public var savedOrigin: Int = 0
    public var role: ZdParameterRole = .member
    public var pid: Int = 0
    public var mustUnderstand: Bool = false

    public init(
        _ at: Int,
        _ extended: Bool,
        _ savedOrigin: Int,
        _ role: ZdParameterRole,
        _ pid: Int,
        _ mustUnderstand: Bool
    ) {
        self.at = at
        self.extended = extended
        self.savedOrigin = savedOrigin
        self.role = role
        self.pid = pid
        self.mustUnderstand = mustUnderstand
    }
}

/// Everything a two-pass read has to put back.
public struct ZdReaderMark {
    public var at: Int = 0
    public var origin: Int = 0
    public var status: ZdStatus = .ok
}

/// An EMHEADER as read. `extent` is how many octets the member occupies
/// after the header word, the NEXTINT included. XTypes 1.3 §7.4.3.4.2.
public struct ZdEmHeaderRead {
    public var memberId: Int = 0
    public var lengthCode: Int = 0
    public var mustUnderstand: Bool = false
    public var nextInt: Int = -1
    public var extent: Int = 0
}

/// A parameter header as read.
public struct ZdPidRead {
    public var pid: Int = 0
    public var length: Int = 0
    public var end: Bool = false
    public var mustUnderstand: Bool = false
    public var implExtension: Bool = false
    public var reserved: Bool = false
    public var ignore: Bool = false
    public var role: ZdParameterRole = .member
    public var savedOrigin: Int = 0
    /// The first octet after the header — §7.4.1.2 counts a member's
    /// internal alignment from here, and `finishPidRead` measures the
    /// value's length against it.
    public var valueStart: Int = 0

    /// Whether this parameter can be a member of the reading type:
    /// false for a vendor-private parameter and for a reserved id.
    public var isUserMember: Bool {
        return !implExtension && !reserved && !ignore
    }
}

/// The walk's spelling of the membership question, as a free function.
public func zdPidIsUserMember(_ header: ZdPidRead) -> Bool {
    return header.isUserMember
}
